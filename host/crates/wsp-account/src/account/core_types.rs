// SPDX-License-Identifier: AGPL-3.0-only
//! What every unit of the account slice shares: the error a unit answers, the frame and road shapes, and the live
//! entry, which is wsp-machines' `Entry` over wsp-core's `LiveWorkspace`.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use serde_json::{Map, Value};

use wsp_core::types::internal::DaemonRefusal;
use wsp_words::exit::HostError;

/// A daemon frame or reply as the TS object, keys in the order they were set (README A8).
pub use wsp_core::reach::Frame;

/// A workspace this host holds in memory, as every area holds it (wsp-machines' `Entry`).
pub use wsp_machines::machines::machines::Entry;

/// What a guard or an owned process hands back: calling it gives the place back.
pub type Release = Rc<dyn Fn()>;

/// The error an account area answers: a host error as the TS `Error` (README A4), or a daemon's refusal with the code
/// it put on it (`DaemonRefusal`, types/internal.ts:1074), which a caller tells apart by its code.
#[derive(Clone, Debug)]
pub enum AccountError {
    Host(HostError),
    Refusal(DaemonRefusal),
}

impl AccountError {
    /// `e instanceof Error ? e.message : String(e)`.
    pub fn message(&self) -> String {
        todo!()
    }

    /// The daemon's code where this is a refusal that carried one.
    pub fn code(&self) -> Option<&str> {
        todo!()
    }

    /// `isNoHostCli(e)`: a refusal coded `no-host-cli`.
    pub fn is_no_host_cli(&self) -> bool {
        todo!()
    }

    /// The host error, where this is one.
    pub fn host(&self) -> Option<&HostError> {
        todo!()
    }
}

impl From<HostError> for AccountError {
    fn from(e: HostError) -> Self {
        AccountError::Host(e)
    }
}

impl From<DaemonRefusal> for AccountError {
    fn from(e: DaemonRefusal) -> Self {
        AccountError::Refusal(e)
    }
}

pub type AccountResult<T> = Result<T, AccountError>;

/// One frame to a daemon and its reply passed through `replyOf` (channels.ts:208): the `ask` a road hands its work.
pub type Ask = Rc<dyn Fn(Frame) -> LocalBoxFuture<'static, AccountResult<Frame>>>;

/// A machine that answers its own daemon frames over its computer's link (`machine.daemonFrame`): the reply as the
/// computer wrote it, `ok: false` included.
pub type Served = Rc<dyn Fn(Frame) -> LocalBoxFuture<'static, AccountResult<Frame>>>;

/// The work a road runs with its `ask`.
pub type Work<T> = Box<dyn FnOnce(Ask) -> LocalBoxFuture<'static, AccountResult<T>>>;
