// SPDX-License-Identifier: AGPL-3.0-only
//! The persistence port of packages/runtime/src/store.ts:9-31 at 65f6b89c7: signatures only. The crate table puts
//! store.ts in wsp-store; it sits here until wsp-store carries serde_json, and moving it is a path change. Values
//! behave as rule 2.27 says: every read is parsed afresh, `get` of a missing id is `None`, `list` and `keys` of an
//! unknown collection are empty, `delete` of a missing row does nothing.
use async_trait::async_trait;
use serde_json::Value;

/// A store call that failed: the backing store's own words.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct StoreError(pub String);

// store.ts:26
// export interface BlobMark { bytes: number; at: number; }
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlobMark {
    pub bytes: u64,
    pub at: f64,
}

// store.ts:9
// export interface Store {
//   get(collection: string, id: string): Promise<unknown | undefined>;
//   put(collection: string, id: string, value: unknown): Promise<void>;
//   list(collection: string): Promise<unknown[]>;
//   keys(collection: string): Promise<string[]>;
//   delete(collection: string, id: string): Promise<void>;
//   getBlob(collection: string, id: string): Promise<Buffer | undefined>;
//   putBlob(collection: string, id: string, bytes: Buffer): Promise<void>;
//   deleteBlob(collection: string, id: string): Promise<void>;
//   statBlob(collection: string, id: string): Promise<BlobMark | undefined>;
//   transcripts?: TranscriptRows;
// }
/// `transcripts` joins when sqlite-transcripts.ts is ported.
#[async_trait(?Send)]
pub trait Store {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, StoreError>;
    async fn put(&self, collection: &str, id: &str, value: &Value) -> Result<(), StoreError>;
    /// The collection's documents in the store's own order.
    async fn list(&self, collection: &str) -> Result<Vec<Value>, StoreError>;
    async fn keys(&self, collection: &str) -> Result<Vec<String>, StoreError>;
    async fn delete(&self, collection: &str, id: &str) -> Result<(), StoreError>;
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, StoreError>;
    async fn put_blob(&self, collection: &str, id: &str, bytes: &[u8]) -> Result<(), StoreError>;
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), StoreError>;
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, StoreError>;
}
