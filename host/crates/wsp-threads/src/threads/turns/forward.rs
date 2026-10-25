// SPDX-License-Identifier: AGPL-3.0-only
//! turns.ts:291-569: a turn's adapter events turned into rows. Spec: spec/turns_forward.md.
use std::rc::Rc;

use wsp_adapters::port::AdapterEvent;

use super::run::TurnState;

impl TurnState {
    /// turns.ts:291. Rules F1-F18.
    pub fn forward(self: &Rc<Self>, event: AdapterEvent) {
        todo!()
    }
}
