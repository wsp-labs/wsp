// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/forwards.ts at 65f6b89c7: every port forward source as one. Spec: spec/sockets.md.
use std::rc::Rc;

use wsp_wire::views::place::{ForwardCloseEvent, ForwardOpenEvent};
use wsp_wire::wire::limits::PortForward;

use crate::types::wiring::Detach;

/// protocol views/place.ts:17 `ForwardEvent`, which wsp-wire carries as its two halves only.
#[derive(Clone, Debug, PartialEq)]
pub enum ForwardEvent {
    Open(ForwardOpenEvent),
    Close(ForwardCloseEvent),
}

/// The port forwards a host holds, as the app lists and stops them. forwards.ts:9.
pub trait ForwardsSource {
    fn list(&self) -> Vec<PortForward>;
    /// True when a forward was open on that workspace and port and is now closed.
    fn stop(&self, workspace_id: &str, port: i64) -> bool;
    fn on(&self, listener: Rc<dyn Fn(&ForwardEvent)>) -> Detach;
}

/// Every source as one: listed in the order given, stopped by the first that holds the forward (the rest are not
/// asked), heard from each; `None` sources are dropped. forwards.ts:17.
pub fn forwards_of(sources: Vec<Option<Rc<dyn ForwardsSource>>>) -> Rc<dyn ForwardsSource> {
    todo!()
}
