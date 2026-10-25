// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/app-ports.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

/// TS: packages/protocol/src/app-ports.ts:10 at a3b870a6e.
pub const DEFAULT_PORT: u64 = 4_400;

/// TS: packages/protocol/src/app-ports.ts:13 at a3b870a6e.
pub const PLACE_PORT_OFFSET: u64 = 20;

/// TS: packages/protocol/src/app-ports.ts:15 at a3b870a6e.
pub const DEFAULT_PLACE_PORT: u64 = 4_420;

/// TS: packages/protocol/src/app-ports.ts:19 at a3b870a6e.
pub const LOOPBACK: &str = "127.0.0.1";

/// TS: packages/protocol/src/app-ports.ts:23 at a3b870a6e.
pub const WS_PATH: &str = "/ws";

/// TS: packages/protocol/src/app-ports.ts:65 at a3b870a6e.
pub const WILDCARD: &str = "0.0.0.0";

/// TS: packages/protocol/src/app-ports.ts:187 at a3b870a6e.
pub const PORT_TAKEN_REFUSAL: &str = "Nothing was booted. Stop that process, or name a free port with --port.";
