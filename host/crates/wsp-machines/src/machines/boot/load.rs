// SPDX-License-Identifier: AGPL-3.0-only
//! boot.ts:16-326 at 65f6b89c7: builders and hydration. Spec: spec/boot_load.md.

use std::rc::Rc;
use std::sync::Arc;

use wsp_core::types::internal::{BuilderLife, StoredBuilder};
use wsp_core::types::wiring::FoundMachine;
use wsp_engine::machine::Machine;
use wsp_words::exit::HostError;
use wsp_words::words::thread::GoneSeenBy;

use super::BootArea;
use crate::machines::machines::Entry;

/// How long the load waits on the provider's reads of its records' machines, all together (boot.ts:18).
pub const LOAD_READS_MS: u64 = 1_500;

/// A provider read out, shared by the load's timer and the late load (boot.ts:149 `asked`).
pub type Asked = futures::future::Shared<futures::future::LocalBoxFuture<'static, Result<Arc<dyn Machine>, HostError>>>;

/// The load's one deadline and every read still out at it, by record id (boot.ts:21).
pub struct LoadReads {
    pub until: i64,
    pub late: std::cell::RefCell<indexmap::IndexMap<String, Asked>>,
}

/// `lateReadLine` (boot.ts:27).
pub fn late_read_line(machine_id: &str) -> String {
    todo!()
}

impl BootArea {
    /// `sealableAt` (boot.ts:44), spec rule 2.
    pub fn sealable_at(&self, place: Option<&str>, first_life: bool) -> bool {
        todo!()
    }

    /// `lifeOf` (boot.ts:49), spec rule 3. `labels` are the machine's provider labels.
    pub fn life_of(&self, stored: &StoredBuilder, labels: Option<&indexmap::IndexMap<String, String>>, sealable: bool) -> BuilderLife {
        todo!()
    }

    /// `admitOnce` (boot.ts:111) over `admit` (boot.ts:78), spec rules 5 and 6.
    pub async fn admit_once(self: &Rc<Self>, stored: StoredBuilder) -> Result<(), HostError> {
        todo!()
    }

    /// `refreshBuilders` (boot.ts:128), spec rule 7.
    pub async fn refresh_builders(self: &Rc<Self>) -> Result<(), HostError> {
        todo!()
    }

    /// `isHeldAway` (boot.ts:190), spec rule 10.
    pub fn is_held_away(&self, id: &str) -> bool {
        todo!()
    }

    /// `rereadHeld` (boot.ts:200), spec rule 11.
    pub async fn reread_held(self: &Rc<Self>, id: &str, by: GoneSeenBy) -> Result<(), HostError> {
        todo!()
    }

    /// `hydrateWorkspace` (boot.ts:229), spec rule 12. `asked` is a read already out (the late load's).
    pub async fn hydrate_workspace(
        self: &Rc<Self>,
        raw: serde_json::Value,
        seen: Option<FoundMachine>,
        load: Option<Rc<LoadReads>>,
        asked: Option<Asked>,
    ) -> Result<Option<Entry>, HostError> {
        todo!()
    }

    /// `loadLate` (boot.ts:170), spec rule 9.
    pub async fn load_late(self: &Rc<Self>, id: &str, read: Asked) -> Option<Entry> {
        todo!()
    }
}
