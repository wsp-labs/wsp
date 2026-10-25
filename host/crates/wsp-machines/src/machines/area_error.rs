// SPDX-License-Identifier: AGPL-3.0-only
//! A failure as the machines-b areas pass it between them. `wsp-words`' `HostError` has no `status` or `cause` and
//! `wsp-engine` has no `From<EngineError> for HostError` yet, so an engine road's own error is kept whole beside the
//! host error until both land; then this file goes and `AreaResult<T>` becomes `Result<T, HostError>`.
use wsp_engine::errors::EngineError;
use wsp_words::exit::HostError;

/// An engine road's own error, whole so `fault_of` and `is_missing` can read its variant, or a host error with its
/// kind. The TS throws either through the same `catch`.
#[derive(Debug, Clone)]
pub enum AreaError {
    Engine(EngineError),
    Host(HostError),
}

impl AreaError {
    /// `e instanceof Error ? e.message : String(e)`: the engine error's Display, or the host error's message.
    pub fn message(&self) -> String {
        todo!()
    }
    /// The kind stamped on it: the engine error's kind string, or the host error's kind, as the TS string.
    pub fn kind(&self) -> Option<String> {
        todo!()
    }
    /// The `fix` a refusal carries; none for an engine error.
    pub fn fix(&self) -> Option<String> {
        todo!()
    }
    /// engine errors.ts `isMissing`: the engine's own reading, or a host error of kind `missing`.
    pub fn is_missing(&self) -> bool {
        todo!()
    }
    /// types/internal.ts:879 `isCapRefusal`: kind `concurrency`.
    pub fn is_cap_refusal(&self) -> bool {
        todo!()
    }
    /// engine errors.ts `failureWords` for an engine error; the message for a host error.
    pub fn failure_words(&self) -> String {
        todo!()
    }
    /// At the area's edge: the same words and kind as a `HostError`.
    pub fn into_host(self) -> HostError {
        todo!()
    }
}

impl From<HostError> for AreaError {
    fn from(e: HostError) -> Self {
        AreaError::Host(e)
    }
}

impl From<EngineError> for AreaError {
    fn from(e: EngineError) -> Self {
        AreaError::Engine(e)
    }
}

pub type AreaResult<T> = Result<T, AreaError>;
