// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/images/` at 65f6b89c7: the records area (records.ts), the held images (held.ts), the builders
//! area (builders.ts), the golden area (golden.ts) and the image area (image.ts). Specs: `spec/images_records.md`,
//! `builders_places.md`, `builders_life.md`, `golden_prepare.md`, `golden_upgrade.md`, `golden_storage.md`,
//! `image.md`, and the shared rules in `spec/machines-rest-README.md`.

pub mod builders;
pub mod engine;
pub mod golden;
pub mod held;
pub mod image;
pub mod records;

use std::cell::RefCell;
use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;

use wsp_core::types::internal::{BuilderLife, BuilderRecord, PrepareStoppedError};
use wsp_core::types::wiring::GoldenRecipe;
use wsp_engine::errors::EngineError;
use wsp_words::exit::{HostError, Kind};
use wsp_wire::wire::machine_link::PreviewReach;

pub use engine::EngineBuilder;

/// A failure as the image areas pass it: a host error with its kind, an engine road's own error whole (so
/// `NotFirstLife`, a missing machine and the place-absent error stay apart), a stopped prepare, or the engine's
/// `SnapshotFailedError` (golden.ts:65) with the state its builder was read in.
#[derive(Clone, Debug)]
pub enum ImageError {
    Host(HostError),
    Engine(EngineError),
    Stopped(PrepareStoppedError),
    SnapshotFailed { message: String, builder_state: String },
}

impl ImageError {
    /// `e instanceof Error ? e.message : String(e)`.
    pub fn message(&self) -> String {
        todo!()
    }
    /// The kind stamped on it: the host error's, the engine error's kind word, `snapshotFailed`; none for a stop.
    pub fn kind(&self) -> Option<Kind> {
        todo!()
    }
    /// engine errors.ts `isMissing`: the engine's reading, or a host error of kind `missing`.
    pub fn is_missing(&self) -> bool {
        todo!()
    }
    /// engine link-backend.ts `isPlaceAbsent`.
    pub fn is_place_absent(&self) -> bool {
        todo!()
    }
    /// The same words and kind as a `HostError`, at the area's edge.
    pub fn into_host(self) -> HostError {
        todo!()
    }
}

impl From<HostError> for ImageError {
    fn from(e: HostError) -> Self {
        ImageError::Host(e)
    }
}

impl From<EngineError> for ImageError {
    fn from(e: EngineError) -> Self {
        ImageError::Engine(e)
    }
}

impl From<PrepareStoppedError> for ImageError {
    fn from(e: PrepareStoppedError) -> Self {
        ImageError::Stopped(e)
    }
}

pub type ImageResult<T> = Result<T, ImageError>;

/// A future a second caller joins (TS: the same `Promise` handed to both).
pub type Joined<T> = Shared<LocalBoxFuture<'static, ImageResult<T>>>;

/// internal.ts:923 `LiveBuilder`: a builder as this host holds it, the one type the boot's load and the image areas
/// share.
pub use wsp_core::types::internal::LiveBuilder;

/// One builder, shared and changed in place; two refs are the same builder only where `Rc::ptr_eq`.
pub type BuilderRef = Rc<RefCell<LiveBuilder>>;

/// `ctx.builders`: every builder this host holds, by machine id, in insertion order.
pub type Builders = Rc<RefCell<IndexMap<String, BuilderRef>>>;
