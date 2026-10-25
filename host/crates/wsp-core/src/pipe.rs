// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/pipe.ts at 65f6b89c7: the host's pool of pipes to each brokered machine's daemon and
//! the area that runs it. Two units, each with its spec under spec/: `pool` (pipe_pool.md) and `area` (pipe_area.md).
pub mod area;
pub mod pool;

pub use area::{cut_off, pipe_area, PipeArea, PipeAreaContext, PipeBroker, PipeLive};
pub use pool::{
    pipe_pool, token_refusals, token_refused_line, PipeDuplex, PipeHandler, PipeIo, PipePool, PipePoolOptions, PipeReach, TokenRefusals,
    PIPE_BEHIND_AGAIN_MS, PIPE_DIAL_MS, PIPE_DIAL_RATE, PIPE_IDLE, PIPE_IDLE_MAX_MS, TOKEN_REFUSED_MAX_MS,
};
