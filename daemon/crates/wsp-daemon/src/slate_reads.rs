// SPDX-License-Identifier: AGPL-3.0-only
//! What a slate asks of the thread's own computer: an image or a frame's page by its whole path, the hashes of the files a command
//! names, and a message posted to an http MCP server from there (http_post.rs). Typed and refused as every other
//! frame the daemon serves.

use std::sync::Arc;

use serde_json::Value;
use wsp_frames::{DaemonErrorCode, DaemonOp, RequestId};

use crate::ops::{answer, not_built, refuse, Conn};
use crate::Ctx;

pub(crate) async fn serve(conn: &Arc<Conn>, ctx: &Ctx, id: Option<RequestId>, name: &str, frame: &Value) -> String {
    if name == "http.post" {
        return crate::http_post::serve(conn, ctx, id, name, frame).await;
    }
    match serde_json::from_value::<DaemonOp>(frame.clone()) {
        Ok(DaemonOp::FsImage { path, machine_id }) => answer(id, crate::image::image_of(ctx, machine_id.as_deref(), path).await),
        Ok(DaemonOp::FsPage { path, machine_id }) => answer(id, crate::image::page_of(ctx, machine_id.as_deref(), path).await),
        Ok(DaemonOp::FsHash { root, paths, machine_id }) => answer(id, crate::hash::hash_of(ctx, machine_id.as_deref(), root, paths).await),
        Ok(_) => refuse(id, DaemonErrorCode::Unsupported, not_built(name)),
        Err(e) => refuse(id, DaemonErrorCode::BadRequest, e.to_string()),
    }
}
