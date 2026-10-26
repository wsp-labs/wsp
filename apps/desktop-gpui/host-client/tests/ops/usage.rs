// SPDX-License-Identifier: AGPL-3.0-only
use serde_json::json;
use wsp_host_client::protocol::*;

use crate::common::{answered, ask, first, refused, shared};

#[tokio::test]
async fn usage() {
    let client = shared().host.client().await;
    let first = first(&client).await;

    let used = answered("usage.used", client.call(ask::<UsageUsedRequest>(json!({ "range": "week", "split": "agent" }))).await).used;
    assert_eq!((used.range, used.split), (UsedAnswerRange::Week, UsedAnswerSplit::Agent));
    assert_eq!(used.series.len(), 7, "{:?}", used.series);

    answered("usage.accounts", client.call(ask::<UsageAccountsRequest>(json!({ "fresh": true }))).await);

    // No agent is signed in on a throwaway host, so there is no account to spend a reset on.
    refused("usage.reset", client.call(ask::<UsageResetRequest>(json!({ "account": "nobody" }))).await, "no account nobody");

    // The readings are the daemon's, and a throwaway host stages none.
    refused(
        "places.readings",
        client.call(ask::<PlacesReadingsRequest>(json!({ "placeId": "here", "range": "15m" }))).await,
        "the daemon for this computer did not start",
    );

    let history = answered("cost.history", client.call(ask::<CostHistoryRequest>(json!({ "workspaceId": first.workspace_id }))).await);
    assert!(!history.points.is_empty() && history.points.iter().all(|p| p.workspace_id == first.workspace_id), "{:?}", history.points);
}
