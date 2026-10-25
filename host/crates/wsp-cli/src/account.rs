// SPDX-License-Identifier: AGPL-3.0-only
//! The three files this computer keeps about an account: a linked host's relay record beside its state, a person's
//! own client record in the wsp home, and the device key that computer signs with. Port of
//! `packages/host/src/account.ts` at 65f6b89c7; the spec is `spec/account.md`.
use serde::{Deserialize, Serialize};
use wsp_store::keys::PlaceKeyPair;

/// account.ts:15-18.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceKey {
    pub fingerprint: String,
    pub public_key: String,
}

/// account.ts:22-37. Read with only `relayUrl`, `hostId` and `token` checked as strings; the other fields are kept as
/// the file holds them (any JSON value), since the TS hands the parsed object on unchecked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayRecord {
    pub relay_url: String,
    pub host_id: String,
    pub token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_key: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_at: Option<serde_json::Value>,
}

/// account.ts:40-50. Read with `relayUrl` and `token` strings and `hostId` absent (account.ts:60 reads
/// `hostId === undefined`, so a `hostId` of `null` or any other value makes it no client record).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayClientRecord {
    pub relay_url: String,
    pub token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_at: Option<serde_json::Value>,
}

/// account.ts:76-78: `JSON.stringify(value, null, 2)` and a newline through `write_own(dirname, basename)`.
pub fn write_json_file(path: &str, value: &impl Serialize) -> std::io::Result<()> {
    todo!()
}

/// account.ts:80-82.
pub fn relay_record_path(state_path: &str) -> String {
    todo!()
}

/// account.ts:84-86.
pub fn read_relay_record(state_path: &str) -> Option<RelayRecord> {
    todo!()
}

/// account.ts:88-90.
pub fn write_relay_record(state_path: &str, record: &RelayRecord) -> std::io::Result<()> {
    todo!()
}

/// account.ts:92-94: a missing file is no error.
pub fn remove_relay_record(state_path: &str) -> std::io::Result<()> {
    todo!()
}

/// account.ts:98-100.
pub fn client_record_path(home: &str) -> String {
    todo!()
}

/// account.ts:102-104.
pub fn read_relay_client(home: &str) -> Option<RelayClientRecord> {
    todo!()
}

/// account.ts:106-108.
pub fn write_relay_client(home: &str, record: &RelayClientRecord) -> std::io::Result<()> {
    todo!()
}

/// account.ts:110-112: a missing file is no error.
pub fn remove_relay_client(home: &str) -> std::io::Result<()> {
    todo!()
}

/// account.ts:114-116.
pub fn device_key_path(home: &str) -> String {
    todo!()
}

/// account.ts:118-120: `publicKey` and `privateKeyPem` strings, else nothing.
pub fn read_device_key_pair(home: &str) -> Option<PlaceKeyPair> {
    todo!()
}

/// account.ts:125-131: the pair held, else a fresh one written as `{"publicKey", "privateKeyPem"}` and answered.
pub fn device_key_here(home: &str) -> std::io::Result<PlaceKeyPair> {
    todo!()
}

/// account.ts:134.
pub fn device_key_of(pair: &PlaceKeyPair) -> DeviceKey {
    todo!()
}
