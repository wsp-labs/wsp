// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-images.ts at 65f6b89c7. Spec: spec/slate_images.md (rules 1-6).
//!
//! A slate's image piece, read for the window: a path from any folder on the thread's computer, this one's disk or
//! another computer's through its daemon, or an address the host fetches once the person allows its domain. Only a
//! regular file whose bytes are an image type and that weighs at most the cap shows; each answer carries its version;
//! a thread reads at most four images at once.
#![allow(unused_variables)]

use std::rc::Rc;

use futures::future::LocalBoxFuture;

pub use wsp_wire::wire::daemon::FsImageReply;

use crate::pending::wire::SlatesImageAnswer;
use crate::slate_image_fetch::FetchRoad;

/// How many images one thread reads or fetches at once (slate-images.ts:189).
pub const IMAGES_AT_ONCE: usize = 4;

/// A daemon's refusal of an `fs.image` read: its code where it sent one, and its words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRefusal {
    pub code: Option<String>,
    pub message: String,
}

/// The thread's computer where it is not this one (slate-images.ts:192).
#[derive(Clone)]
pub struct ImageOn {
    pub name: String,
    pub read: Rc<dyn Fn(String) -> LocalBoxFuture<'static, Result<FsImageReply, ImageRefusal>>>,
}

/// slate-images.ts:197 SlateImageRoads.
#[derive(Clone)]
pub struct SlateImageRoads {
    pub thread: String,
    /// The version the window holds.
    pub have: Option<String>,
    /// The thread's folder on its own computer, which a relative path is read under.
    pub folder: Option<String>,
    /// Absent where the thread runs on this computer.
    pub on: Option<ImageOn>,
    /// Whether the person allowed this domain for the thread.
    pub allowed: Rc<dyn Fn(&str) -> bool>,
    pub fetch: Option<Rc<dyn FetchRoad>>,
}

// slate-images.ts:226 slateImage(src, roads)
pub async fn slate_image(src: &str, roads: SlateImageRoads) -> SlatesImageAnswer {
    todo!()
}

// slate-images.ts:247 versionOf(modifiedMs, size, inode, changedNs)
/// `round(modified):size[:inode][:changed]` (rule 4).
pub fn version_of(modified_ms: f64, size: u64, inode: Option<u64>, changed_ns: Option<u128>) -> String {
    todo!()
}
