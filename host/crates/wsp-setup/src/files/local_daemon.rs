// SPDX-License-Identifier: AGPL-3.0-only
//! `local-daemon.ts`: the daemon for this computer's own workspace, spawned on loopback and reached by the same link as a cloud one.
use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use wsp_core::reach::{DaemonReach, ReachError};
use wsp_wire::types::JsNumber;
use wsp_wire::views::workspace::DaemonReachView;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::machine_link::SysSample;
use wsp_words::exit::HostError;

/// `LocalDaemonOptions` (`local-daemon.ts:73`).
pub struct LocalDaemonOptions {
    pub root: String,
    pub work_folder: String,
    pub roots_path: String,
    pub inbox_dir: String,
    pub readings_dir: Option<String>,
    pub docker_notes: Option<String>,
    pub binary: Option<String>,
    pub token_path: Option<String>,
    pub say: Option<Rc<dyn Fn(&str)>>,
    /// The child is handed over as an `Rc`, since the callee keeps it and a tokio child is killed through `&mut`.
    pub spawned: Option<Rc<dyn Fn(Rc<tokio::process::Child>)>>,
}

/// `SharedSamples` (`local-daemon.ts:137`), the one watch on this computer's readings that every pane shares.
pub struct SharedSamples {
    pub link: Rc<dyn DaemonReach>,
    /// A list rather than a set: a closure has no hash, and a listener leaves by `Rc::ptr_eq`.
    pub listeners: Vec<Rc<dyn Fn(&SysSample)>>,
    pub watching: Shared<LocalBoxFuture<'static, Result<(), HostError>>>,
}

/// `LocalDaemon` (`local-daemon.ts:145`), the daemon for this computer's workspace on loopback with a fresh token.
pub struct LocalDaemon {
    pub samples: Option<SharedSamples>,
    pub child: tokio::process::Child,
    pub exited: Shared<LocalBoxFuture<'static, ()>>,
    /// The folder holding the token file and the manifest, gone with the daemon.
    pub own_dir: String,
    pub minted: String,
    pub port: u16,
    pub version: JsNumber,
    pub root: String,
    pub token_path: Option<String>,
}

impl LocalDaemon {
    /// `LocalDaemon.start` (`local-daemon.ts:175`).
    pub async fn start(_opts: LocalDaemonOptions) -> Result<LocalDaemon, HostError> {
        todo!()
    }

    /// `LocalDaemon.pid` (`local-daemon.ts:260`).
    pub fn pid(&self) -> u32 {
        todo!()
    }

    /// `LocalDaemon.road` (`local-daemon.ts:267`).
    pub fn road(&self) -> DaemonReachView {
        todo!()
    }

    /// `LocalDaemon.link` (`local-daemon.ts:273`).
    pub fn link(&self, _on_event: Rc<dyn Fn(&DaemonEvent)>) -> Result<Rc<dyn DaemonReach>, ReachError> {
        todo!()
    }

    /// `LocalDaemon.sysSamples` (`local-daemon.ts:280`), answering the detach that stops this listener.
    pub async fn sys_samples(&self, _listener: Rc<dyn Fn(&SysSample)>) -> Result<Rc<dyn Fn()>, HostError> {
        todo!()
    }

    /// `LocalDaemon.close` (`local-daemon.ts:306`).
    pub async fn close(&self) {
        todo!()
    }
}
