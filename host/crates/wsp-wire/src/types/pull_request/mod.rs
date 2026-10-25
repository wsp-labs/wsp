// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/pull-request.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use crate::literal;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::views::session::ThreadCapWait;
use serde::{Deserialize, Serialize};
/// TS: packages/protocol/src/pull-request.ts:19 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::CheckState;
/// TS: packages/protocol/src/pull-request.ts:22 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::MergeMethod;
/// TS: packages/protocol/src/pull-request.ts:13 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::Mergeable;
/// TS: packages/protocol/src/pull-request.ts:10 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::PullRequestState;
/// TS: packages/protocol/src/pull-request.ts:394 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::ReactionContent;
/// TS: packages/protocol/src/pull-request.ts:16 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::ReviewState;

pub use crate::types::frames_replies::{GitPrMergeReply, GitPrResolveReply, GitRepoReadReply, GitRunLogReply, PullRequestReviewRequest};

mod pr_poll_ms;
mod pull_request_comment;

pub use pr_poll_ms::*;
pub use pull_request_comment::*;
