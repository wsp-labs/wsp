// SPDX-License-Identifier: AGPL-3.0-only
//! The drivers of the `ws_reads` unit: one per recorded helper, each reading the case's arguments, calling the Rust
//! helper and answering the JSON the TS value was written as. `workspaces-help.ts:111-296`, `threadRows`, `branchHere`.
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use wsp_cli::verbs::client::{BoxFuture, Out};
use wsp_cli::verbs::workspaces_help as w;
use wsp_words::exit::HostError;

use super::common::Env;

fn typed<T: DeserializeOwned>(env: &Env, name: &str) -> T {
    serde_json::from_value(env.arg(name).unwrap_or_else(|| panic!("the case names no {name}")).clone()).unwrap_or_else(|e| panic!("the recorded {name} reads: {e}"))
}

fn said<T: serde::Serialize>(value: T) -> Result<Value, HostError> {
    Ok(serde_json::to_value(value).expect("the value serializes"))
}

pub fn workspaces(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::workspaces(env.client.as_ref()).await?) })
}

pub fn workspace_statuses(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::workspace_statuses(env.client.as_ref()).await?) })
}

pub fn threads(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::threads(env.client.as_ref(), env.str_arg("workspaceId").as_deref()).await?) })
}

pub fn thread_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::thread_of(env.client.as_ref(), &env.str_arg("ref").expect("a ref")).await?) })
}

pub fn threads_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::threads_of(env.client.as_ref(), &env.strs("refs")).await?) })
}

pub fn workspace_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::workspace_of(env.client.as_ref(), &env.str_arg("ref").expect("a ref")).await?) })
}

pub fn thread_rows(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::thread_rows(env.client.as_ref(), env.str_arg("within").as_deref()).await?) })
}

pub fn state_line(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::state_line(&typed(env, "workspace"))) })
}

pub fn rebuilt_line(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::rebuilt_line(&typed(env, "workspace"))) })
}

pub fn woke_line(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::woke_line(env.client.as_ref(), &typed(env, "workspace")).await?) })
}

pub fn nap(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::nap(env.client.as_ref(), &env.str_arg("ref").expect("a ref")).await?) })
}

pub fn resize(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::resize(env.client.as_ref(), &env.str_arg("ref").expect("a ref"), &typed(env, "size")).await?) })
}

pub fn rebuild(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::rebuild(env.client.as_ref(), &env.str_arg("ref").expect("a ref")).await?) })
}

pub fn image_view(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::image_view(env.client.as_ref()).await?) })
}

pub fn image_lines(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::image_lines(&typed(env, "view"))) })
}

pub fn build_image_at(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let out = Out::formatter(env.sinks.clone(), false);
        let force = env.arg("force").and_then(|v| v.as_bool());
        said(w::build_image_at(env.client.as_ref(), &out, &env.str_arg("word").expect("a word"), force).await?)
    })
}

pub fn const_host_side_vault(_env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { Ok(json!(w::HOST_SIDE_VAULT)) })
}

pub fn const_ssh_pipes_here_line(_env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { Ok(json!(w::SSH_PIPES_HERE_LINE)) })
}

pub fn ssh_workspace_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::ssh_workspace_of(env.client.as_ref(), &env.str_arg("ref").expect("a ref")).await?) })
}
