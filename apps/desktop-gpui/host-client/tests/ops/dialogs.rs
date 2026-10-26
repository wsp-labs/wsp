// SPDX-License-Identifier: AGPL-3.0-only
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
async fn cloud_setup() {
    let client = shared().host.client().await;
    let setup = answered("init.get", client.call(ask::<InitGetRequest>(json!({}))).await).setup;
    assert!(setup.pricing.is_none());

    let got = client.call(ask::<InitKeysRequest>(json!({ "key": "not-a-key" }))).await;
    refused("init.keys", got, "this host forks machines on a provider that reads no API key, so there is none to save");
    answered("init.keys", client.call(ask::<InitKeysRequest>(json!({}))).await);
}
