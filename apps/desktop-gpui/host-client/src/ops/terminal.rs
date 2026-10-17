// SPDX-License-Identifier: AGPL-3.0-only
use serde::Deserialize;

use crate::{protocol, Done};

ops! {
    "daemon.open" => DaemonOpenRequest => protocol::DaemonOpenReply,
    "daemon.send" => DaemonSendRequest => protocol::DaemonSendReply,
    "daemon.close" => DaemonCloseRequest => Done,
    "sys.subscribe" => SysSubscribeRequest => Done,
    "sys.unsubscribe" => SysUnsubscribeRequest => Done,
    "workspaces.touch" => WorkspacesTouchRequest => Done,
    "host.terminalConfig" => HostTerminalConfigRequest => TerminalConfig,
    "forwards.list" => ForwardsListRequest => Forwards,
    "forwards.stop" => ForwardsStopRequest => Done,
    "workspaces.portReach" => WorkspacesPortReachRequest => PortReach,
    "workspaces.portProbe" => WorkspacesPortProbeRequest => PortProbe,
}

#[derive(Debug, Deserialize)]
pub struct TerminalConfig {
    pub config: protocol::TerminalConfig,
}

#[derive(Debug, Deserialize)]
pub struct Forwards {
    pub forwards: Vec<protocol::PortForward>,
}

#[derive(Debug, Deserialize)]
pub struct PortReach {
    pub reach: protocol::PortReachView,
}

#[derive(Debug, Deserialize)]
pub struct PortProbe {
    pub probe: protocol::PortProbeView,
}
