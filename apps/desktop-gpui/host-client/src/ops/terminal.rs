// SPDX-License-Identifier: AGPL-3.0-only
use serde::Deserialize;

use crate::{protocol, Done};

ops! {
    "daemon.open" => DaemonOpenRequest => protocol::DaemonOpenReply,
    "daemon.send" => DaemonSendRequest => DaemonSent,
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

/// The daemon's answer as it sent it, for the caller to read against the op it asked: its ok and error shapes differ
/// only by a boolean, which typify does not take as a tag.
#[derive(Debug, Deserialize)]
pub struct DaemonSent {
    pub reply: serde_json::Value,
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
