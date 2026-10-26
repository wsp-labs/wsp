// SPDX-License-Identifier: AGPL-3.0-only
//! The host calls the flow asks for, each typed off its params and its reply read into the flow's own shapes. No state
//! lives here: the view awaits these and hands the answers to [`super::flow::Flow`].

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use wsp_host_client::protocol::{InitGetRequest, InitKeysRequest, ProvidersCheckKeyRequest};
use wsp_host_client::{Failure, HostClient, Op};

use super::flow::{Facts, Setup};

async fn call<O: Op + DeserializeOwned>(host: &HostClient, params: Value) -> Result<O::Reply, Failure> {
    let op: O = serde_json::from_value(params).map_err(|e| Failure::new(format!("{}: {e}", O::NAME)))?;
    host.call(op).await
}

fn read<T: DeserializeOwned>(value: impl serde::Serialize) -> Result<T, Failure> {
    serde_json::to_value(value).and_then(serde_json::from_value).map_err(|e| Failure::new(e.to_string()))
}

/// `init.get`, named by no place: a cloud with no key yet is no place, and the host refuses a read that names one.
pub async fn setup(host: &HostClient) -> Result<Setup, Failure> {
    read(call::<InitGetRequest>(host, json!({})).await?.setup)
}

pub async fn check_key(host: &HostClient, provider: String, key: String) -> Result<Facts, Failure> {
    read(call::<ProvidersCheckKeyRequest>(host, json!({ "provider": provider, "key": key })).await?)
}

pub async fn save_key(host: &HostClient, provider: String, key: String) -> Result<Setup, Failure> {
    read(call::<InitKeysRequest>(host, json!({ "provider": provider, "key": key })).await?.setup)
}
