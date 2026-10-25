// SPDX-License-Identifier: AGPL-3.0-only
//! boot.ts:328-621 at 65f6b89c7: the host's start. Spec: spec/boot_ready.md.

use std::rc::Rc;

use wsp_words::exit::HostError;

use super::BootArea;
use crate::machines::machines::Row;

/// The one start every caller of `ready()` awaits; its failure is shared as its words.
pub type Ready = futures::future::Shared<futures::future::LocalBoxFuture<'static, Result<(), String>>>;

impl BootArea {
    /// `ready()` (boot.ts:329): the one start, joined; spec rules 1-14 in order.
    pub fn ready(self: &Rc<Self>) -> Ready {
        todo!()
    }

    /// `reopen` (boot.ts:525), spec rule 10: the rows left running, re-opened by their run handles.
    pub async fn reopen(self: &Rc<Self>, rows: Vec<Row>, again: bool) -> Result<(), HostError> {
        todo!()
    }

    /// `askAgain` (boot.ts:548): the rows asked again one link window on.
    pub fn ask_again(self: &Rc<Self>, rows: Vec<Row>) {
        todo!()
    }
}
