// SPDX-License-Identifier: AGPL-3.0-only
use std::time::{Duration, Instant};

use serde_json::json;
use wsp_host_client::protocol::*;

use crate::common::{answered, ask, refused, shared};

#[tokio::test]
async fn add_a_computer() {
    let client = shared().host.client().await;

    let door = answered("places.door", client.call(ask::<PlacesDoorRequest>(json!({}))).await).door;
    assert!(!door.addresses.is_empty(), "{door:?}");
    let mint = answered("places.mint", client.call(ask::<PlacesMintRequest>(json!({}))).await);
    assert!(mint.joins.iter().all(|join| join.line.starts_with("wsp join ") && join.line.ends_with(door.host_key.as_str())), "{mint:?}");
    let hosts = answered("places.sshHosts", client.call(ask::<PlacesSshHostsRequest>(json!({}))).await);
    assert!(hosts.hosts.is_empty(), "the host's home holds no ssh config: {hosts:?}");

    let code = answered("pair.issue", client.call(ask::<PairIssueRequest>(json!({}))).await);
    assert_eq!(code.host_key.as_deref(), Some(door.host_key.as_str()));
    // A redeem is answered only as the first frame of a socket nothing has authed; this one holds the host's token.
    let got = client.call(ask::<PairRedeemRequest>(json!({ "code": code.code, "name": "dialogs" }))).await;
    refused("pair.redeem", got, "that pairing code is not one this host is waiting for");

    // A port nothing listens on, so the host key scan fails at once and nothing is dialled.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let got = client.call(ask::<PlacesAddRequest>(json!({ "address": "nobody@127.0.0.1", "sshPort": closed }))).await;
    refused("places.add", got, "nobody@127.0.0.1 has never been reached from this computer and answered no key to a scan");

    let picks = json!({ "name": "dialogs" });
    let got = client.call(ask::<PlacesSetupRequest>(json!({ "ref": "dialogs-nowhere", "choices": picks }))).await;
    refused("places.setup", got, "no place named dialogs-nowhere");
    let got = client.call(ask::<PlacesChooseRequest>(json!({ "ref": "dialogs-nowhere", "choices": picks }))).await;
    refused("places.choose", got, "no add of dialogs-nowhere is pending");
    let estimate =
        answered("places.estimate", client.call(ask::<PlacesEstimateRequest>(json!({ "ref": "dialogs-nowhere", "choices": picks }))).await);
    assert_eq!(estimate.estimate.needed_bytes, 0);
}

#[tokio::test]
async fn image_setup() {
    let client = shared().host.client().await;
    let setup = answered("init.get", client.call(ask::<InitGetRequest>(json!({}))).await).setup;
    assert!(setup.pricing.is_none());
    assert!(setup.build_refusal.expect("a refusal with no place to build on").starts_with("no place here runs workspaces"));

    let got = client.call(ask::<InitKeysRequest>(json!({ "key": "not-a-key" }))).await;
    refused("init.keys", got, "this host forks machines on a provider that reads no API key, so there is none to save");
    answered("init.keys", client.call(ask::<InitKeysRequest>(json!({}))).await);

    // The manual road reads this computer and waits on its screens; only init.build would boot a builder.
    let job = answered("init.start", client.call(ask::<InitStartRequest>(json!({ "road": "manual" }))).await).job;
    assert_eq!(job.road.to_string(), "manual");
    let until = Instant::now() + Duration::from_secs(30);
    let job = loop {
        let setup = client.call(ask::<InitGetRequest>(json!({}))).await.unwrap_or_else(|f| panic!("init.get: {}", f.message)).setup;
        let job = setup.job.expect("the job init.start began");
        if job.phase.to_string() != "reading" {
            break job;
        }
        assert!(Instant::now() < until, "the init job is still reading this computer: {job:?}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    assert_eq!(job.phase.to_string(), "answering", "{job:?}");
    let screen = &job.screens[0];
    let id = serde_json::to_value(screen.id).unwrap();

    let draft = json!({ "at": id, "ticks": [] });
    let job = answered("init.draft", client.call(ask::<InitDraftRequest>(draft)).await).job;
    assert!(job.drafts.iter().any(|d| serde_json::to_value(&d.at).unwrap() == id), "{:?}", job.drafts);
    let job = answered("init.answer", client.call(ask::<InitAnswerRequest>(json!({ "screen": id, "ticks": screen.ticks }))).await).job;
    assert_eq!(job.step, 1);
    let job = answered("init.step", client.call(ask::<InitStepRequest>(json!({ "at": 0 }))).await).job;
    assert_eq!(job.step, 0);

    refused("init.build", client.call(ask::<InitBuildRequest>(json!({}))).await, "no place here runs workspaces");
    let code = json!({ "tool": "dialogs-tool", "code": "1234" });
    refused(
        "init.signInCode",
        client.call(ask::<InitSignInCodeRequest>(code)).await,
        "no sign-in for dialogs-tool is waiting for a code from you",
    );
    refused("init.retry", client.call(ask::<InitRetryRequest>(json!({ "tool": "claude" }))).await, "no build is running to sign in on");

    let job = answered("init.cancel", client.call(ask::<InitCancelRequest>(json!({}))).await).job;
    assert_eq!(job.phase.to_string(), "cancelled");
}
