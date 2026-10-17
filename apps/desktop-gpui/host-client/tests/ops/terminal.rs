// SPDX-License-Identifier: AGPL-3.0-only
use serde_json::json;
use wsp_host_client::protocol::*;

use crate::common::{answered, ask, first, refused, shared};

const NO_DAEMON: &str = "the daemon for this computer did not start";

#[tokio::test]
async fn terminal_pane() {
    let client = shared().host.client().await;
    let ws = first(&client).await.workspace_id;
    refused("daemon.open", client.call(ask::<DaemonOpenRequest>(json!({ "workspaceId": ws }))).await, NO_DAEMON);
    refused("daemon.open", client.call(ask::<DaemonOpenRequest>(json!({ "placeId": "here" }))).await, NO_DAEMON);
    let frame = json!({ "channel": "unopened", "frame": { "op": "pty.list" } });
    refused("daemon.send", client.call(ask::<DaemonSendRequest>(frame)).await, "no such daemon channel on this socket");
    refused(
        "daemon.close",
        client.call(ask::<DaemonCloseRequest>(json!({ "channel": "unopened" }))).await,
        "no such daemon channel on this socket",
    );
    answered("workspaces.touch", client.call(ask::<WorkspacesTouchRequest>(json!({ "workspaceId": ws }))).await);

    let ghostty = shared().host.dir.path().join(".config/ghostty");
    std::fs::create_dir_all(&ghostty).unwrap();
    std::fs::write(ghostty.join("config.ghostty"), "font-family = Iosevka\nfont-size = 15\n").unwrap();
    let read = answered("host.terminalConfig", client.call(ask::<HostTerminalConfigRequest>(json!({ "scheme": "dark" }))).await);
    assert_eq!(read.config.font_family, ["Iosevka"]);
    assert_eq!(read.config.font_size.map(f64::from), Some(15.0));
    assert!(read.config.files.iter().any(|f| f.ends_with("ghostty/config.ghostty")), "{:?}", read.config.files);
}

#[tokio::test]
async fn sys_readings() {
    let client = shared().host.client().await;
    let ws = first(&client).await.workspace_id;
    refused("sys.subscribe", client.call(ask::<SysSubscribeRequest>(json!({ "workspaceId": ws }))).await, NO_DAEMON);
    refused("sys.subscribe", client.call(ask::<SysSubscribeRequest>(json!({ "placeId": "here" }))).await, NO_DAEMON);
    answered("sys.unsubscribe", client.call(ask::<SysUnsubscribeRequest>(json!({ "workspaceId": ws }))).await);
    answered("sys.unsubscribe", client.call(ask::<SysUnsubscribeRequest>(json!({ "placeId": "here" }))).await);
}

#[tokio::test]
async fn ports() {
    let client = shared().host.client().await;
    let ws = first(&client).await.workspace_id;
    let port = json!({ "workspaceId": ws, "port": 4567 });
    refused("workspaces.portReach", client.call(ask::<WorkspacesPortReachRequest>(port.clone())).await, "backend without preview URLs");
    refused("workspaces.portProbe", client.call(ask::<WorkspacesPortProbeRequest>(port.clone())).await, "backend without preview URLs");
    let listed = answered("forwards.list", client.call(ask::<ForwardsListRequest>(json!({}))).await);
    assert!(!listed.forwards.iter().any(|f| f.workspace_id == ws && f.port == 4567), "{:?}", listed.forwards);
    refused("forwards.stop", client.call(ask::<ForwardsStopRequest>(port)).await, "nothing is forwarding localhost:4567 there");
}
